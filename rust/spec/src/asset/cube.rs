// pxl-spec/asset/cube.rs

//! The cube kind (`spec/spec/asset/cube.json5`): the ground and the cubes, in one table
//! the GPU reads too, which the value space (`value.rs`) is laid over, a cube as the catalog
//! gives it out to be placed, and what has a cube's body.

use std::sync::LazyLock;

#[path = "cube.spec.rs"]
pub mod spec;
pub use spec::{BASE, GROUND, Ground, Object, PACK, Sentinel};

use crate::asset::color;
use crate::asset::id::{AssetId, IDX_BITS, Kind};

/// Whether a value is matter: everything but void, on which nothing is painted, and air,
/// which is placed as nothing. A ray sees through what is not matter, and it takes no share
/// of its cell. A GPU has no byte type, so every value is tested as a word.
pub const fn is_matter(val: u32) -> bool {
    val != Sentinel::Void as u32 && val != Sentinel::Air as u32
}

/// Whether an asset is matter with a cube's body: a cube is, and so is an id
/// in a cube's range that the catalog does not give out; void and air, a
/// decor and a marker are not.
pub const fn solid(id: AssetId) -> bool {
    is_matter(id) && id >> IDX_BITS == Kind::Cube as u32
}

/// A cube of the catalog (`asset/cube.json5`) is its asset id: one of the
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
