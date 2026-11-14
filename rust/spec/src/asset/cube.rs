// pxl-spec/asset/cube.rs

//! The cube kind (`spec/spec/asset/cube.json`): the ground and the cubes, in one table
//! the GPU reads too, which the value space (value.rs) is laid over, a cube as the catalog
//! gives it out to be placed, and what has a cube's body.

use std::sync::LazyLock;

#[path = "cube.spec.rs"]
mod spec;
pub use spec::{Ground, Object, Sentinel, BASE, GROUND, PACK};

use crate::asset::color;
use crate::asset::id::{AssetId, Kind, IDX_BITS};

/// Whether an asset is matter with a cube's body: a cube is, and so is an id
/// in a cube's range that the catalog does not give out; void and air, a
/// decor and a marker are not.
pub const fn solid(id: AssetId) -> bool {
    id != Sentinel::Void as u32 && id != Sentinel::Air as u32 && id >> IDX_BITS == Kind::Cube as u32
}

/// A cube of the catalog (`asset/cube.json`) is its id: a palette entry of the
/// ground's, a byte, or past a byte one that is only ever placed; a player may
/// place either. None for a sentinel or a decor.
pub fn cube_of(id: AssetId) -> Option<AssetId> {
    let known = match u8::try_from(id) {
        Ok(byte) => Ground::of(byte).is_some(),
        Err(_) => u16::try_from(id).ok().and_then(Object::of).is_some(),
    };
    known.then_some(id)
}

pub fn cube_rgb(id: AssetId) -> Option<u32> {
    match u8::try_from(id) {
        Ok(byte) => color::ground(byte),
        Err(_) => color::object(u16::try_from(id).ok()?),
    }
    .map(|s| s.rgb)
}

/// The placed colors first, then the palette in paint order.
pub static ALL_CUBES: LazyLock<Vec<AssetId>> = LazyLock::new(|| {
    Object::ALL
        .into_iter()
        .map(|o| o as AssetId)
        .chain(Ground::ALL.into_iter().map(|g| g as AssetId))
        .collect()
});

#[cfg(test)]
#[path = "cube.test.rs"]
mod test;
